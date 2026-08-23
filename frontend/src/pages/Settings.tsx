import { Card, Typography } from 'antd';
import { SettingOutlined } from '@ant-design/icons';

const { Title, Text } = Typography;

export default function Settings() {
  return (
    <div className="fade-in-up">
      <Title level={3}><SettingOutlined /> Ajustes</Title>
      <Card>
        <Text type="secondary">
          Los ajustes se configuran mediante variables de entorno.
          Consulta el archivo <code>.env</code> para más detalles.
        </Text>
      </Card>
    </div>
  );
}